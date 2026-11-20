# Security and development status

Run this development build under your own operating-system account. The private state directory holds plaintext journal data, including submitted code and model responses. Trust the process serving the selected loopback port. A local process running as your account can read the journal. Uploaded transcripts and backups carry the same content.

The model receives user-provided text and bounded session history. The runtime exposes no model-invocable OS commands, GitHub credentials, browser sessions or filesystem mutations. Interactive output filters terminal controls and JSON output escapes them.

For a suspected security issue, use an owner-approved private channel. The owner has yet to supply the permanent reporting address required for public release. Avoid putting credentials, transcripts or private source into a public issue.
