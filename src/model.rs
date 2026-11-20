use crate::{store::Store, Result};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read},
    time::Duration,
};

pub const MAX_INPUT: usize = 65536;
const MAX_OUTPUT: usize = 1024 * 1024;

// Only loopback TCP is exposed. The port is data, never a URL supplied by a model.
pub fn run(
    store: &mut Store,
    id: &str,
    model: &str,
    port: u16,
    prompt: &str,
    mut emit: impl FnMut(&str) -> Result<()>,
) -> Result<String> {
    if prompt.trim().is_empty() || prompt.len() > MAX_INPUT {
        return Err("prompt must contain 1–65536 bytes".into());
    }
    if model.trim().is_empty() || model.len() > 256 {
        return Err("provide a model name of 1–256 bytes".into());
    }
    let (turn, messages) = store.begin_turn(id, prompt, model)?;
    let result = (|| {
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .try_proxy_from_env(false)
            .timeout(Duration::from_secs(180))
            .timeout_connect(Duration::from_secs(5))
            .build();
        let body = json!({"model":model,"messages":messages,"stream":true,"max_tokens":1024});
        let res=agent.post(&format!("http://127.0.0.1:{port}/v1/chat/completions")).set("Content-Type","application/json")
            .send_string(&body.to_string()).map_err(|err| match err {ureq::Error::Status(code,_)=>format!("local model returned HTTP {code}"),_=>"local model connection failed; check that the server is running on the selected port".into()})?;
        if res.status() != 200 {
            return Err(format!("unexpected model HTTP status {}", res.status()).into());
        }
        parse_stream(BufReader::new(res.into_reader()), &mut emit)
    })();
    match result {
        Ok(reply) => {
            store.finish(id, turn, Some(&reply))?;
            Ok(reply)
        }
        Err(err) => {
            store.finish(id, turn, None)?;
            Err(err)
        }
    }
}

fn parse_stream(
    mut reader: impl BufRead,
    emit: &mut impl FnMut(&str) -> Result<()>,
) -> Result<String> {
    let mut reply = String::new();
    let mut total = 0;
    loop {
        let mut line = String::new();
        let n = (&mut reader)
            .take((MAX_OUTPUT + 1) as u64)
            .read_line(&mut line)?;
        if n == 0 {
            return Err("model stream ended before [DONE]".into());
        }
        total += n;
        if n > MAX_OUTPUT || total > 16 * MAX_OUTPUT {
            return Err("model stream exceeded its size limit".into());
        }
        let Some(data) = line.trim_end().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            if reply.is_empty() {
                return Err("model returned an empty reply".into());
            }
            return Ok(reply);
        }
        let chunk: Value = serde_json::from_str(data)?;
        if chunk.get("error").is_some() {
            return Err("local model reported a stream error".into());
        }
        if let Some(s) = chunk["choices"][0]["delta"]["content"].as_str() {
            if reply.len() + s.len() > MAX_OUTPUT {
                return Err("model reply exceeded its size limit".into());
            }
            reply.push_str(s);
            emit(s)?;
        }
    }
}
