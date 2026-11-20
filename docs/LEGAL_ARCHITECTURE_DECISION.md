# ADR-019 — Source-available release model

Status: **PRODUCT POLICY SELECTED; LEGAL APPROVAL PENDING**
Date: 2026-11-20

## Decision

License all original StainlessSteel copyright-controlled code under **PolyForm Strict License 1.0.0**, available without license fee for qualifying noncommercial use, but without automatic permissions to modify, distribute or use commercially. Offer discretionary separately written redistribution and commercial rights. Ship third-party components with their own independent licenses, preserving upstream notices. Publish the source on GitHub as **source-available**, not OSI-open-source.

## Why not GPL

GPL would require qualifying derived distributions to honor GPL's permission to redistribute and would permit commercial use. It cannot implement the requested blanket owner permission gate.

## Why standardized PolyForm Strict vs custom text

PolyForm Strict already encodes noncommercial-use-only, no modification and no redistribution. It is more predictable than an unreviewed home-built license, yet it still requires local legal review and careful third-party separation.

## Clarifications and limitations

- Public GitHub repos grant platform-native rights to view and fork; don't promise to eliminate GitHub forks.
- The license does not eliminate independent MIT/Apache/OFL rights in bundled upstream code.
- It restricts modifications to original StainlessSteel code: this complicates open contribution workflows until a narrowly scoped written contributor authorization is issued.
- PolyForm Strict includes a relatively broad noncommercial permitted-purpose definition, including certain nonprofit, educational and government use; legal counsel should check alignment with intended revenue controls.
- Merely learning an idea or independently writing analogous code is not generally prohibited by copyright; protect trademarks/secrets/contractual information separately.
- GitHub hosting may involve AI training access under GitHub Terms; a restrictive copyright license does not automatically undo platform permissions.
- If a prior StainlessSteel release under GPL ever occurred, its existing recipients keep those granted rights; future licensing cannot retroactively revoke them. No such public release is confirmed in this blueprint.
- The source may be publicly accessible, and copying can occur technically even if not licensed; legal terms are not DRM.

## Release status

No compilation or dependency closure has been validated. Licensing is *decided as a policy*, not certified as legally compliant. `docs/PRE_RELEASE_LEGAL_CHECKLIST.md` blocks release.
