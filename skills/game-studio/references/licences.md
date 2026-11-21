# Asset licences

No game ships an asset made by a non-commercial or region-restricted generator. Licences are checked, not remembered.

- The register: `data/licences.tsv` in this skill (generator, version, licence, class, conditions, source, date checked).
  Classes: `allow`, `conditional` (allowed with conditions, e.g. a revenue cap or a visible credit), `deny`.
- The game's side, from `game-studio.json`'s `licences`: `file` (the licence list, e.g. `test/LICENCES.md`) and
  `provenance` (a glob such as `assets/**/provenance.json`, one sidecar per generated asset).
- The check: `game-studio licences [--dir DIR]`.
  - `FAIL game-studio licences: ...` when a `deny` generator is named, or a generator is not in the register
    ("add it to the register"). Not listed is never allowed.
  - `WARN game-studio licences: ...` per `conditional` generator, with its conditions.
  - `PASS game-studio licences: ...` otherwise. Call it from the game's asset gate.

A provenance sidecar:

```json
{"generator": "ACE-Step", "version": "1.5", "prompt": "...", "date": "2026-11-02"}
```

Adding a generator: read its licence and model card yourself, add one row with the source URL and today's date, and
class it `conditional` when anything is unclear. Never class a generator `allow` from a secondary source alone.
