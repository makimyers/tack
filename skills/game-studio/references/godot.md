# Godot verify kit

For a Godot game without its own gates. `game-studio init --engine godot` (with `project.godot` in the repo root)
copies the kit from `~/Projects/tool-claude-setup/templates/game-studio/godot/` into `tools/verify/` (gdUnit4,
fetched at the tag and commit pinned in `gdUnit4.lock`, in `tools/verify/vendor/`),
adds an example test in `test/unit/` when there is none, and sets `verify` to `./tools/verify/all.sh` and
`verify_json` to `.game-studio/verify.json`. A project in a subfolder: set `VERIFY_PROJECT=<folder>` for the gates.

| Gate | Script | Checks |
|---|---|---|
| Import + parse | `import.sh` | the project imports headless with no script errors |
| Scenes | `scenes.sh` | the main scene loads and runs a few frames |
| Unit tests | `unit.sh` | gdUnit4 runs `test/unit/` headless: exit 0 PASS, 100 FAIL, 101 PASS-with-warnings |
| Licences | `all.sh` | `game-studio licences` |
| All | `all.sh` | runs them, prints `PASS`/`FAIL` lines, writes `.game-studio/verify.json` |

Rules:

- `lib.sh` finds Godot 4.7.2 (`GODOT_BIN`, else the flatpak `org.godotengine.Godot`) and copies the project to a temp
  folder first, so Godot never writes `.godot/` into the game.
- gdUnit4 is vendored and pinned (`vendor/gdUnit4.lock`); `unit.sh` adds it and `test/unit/` to the copy only, never
  to the game's project.
- Tests are GDScript files in `test/unit/` extending `GdUnitTestSuite`, functions named `test_*`.
- From a container without Godot, run the gates on the host (for example `distrobox-host-exec ./tools/verify/all.sh`).
- Never weaken a gate or a threshold to make it pass; fix the game or re-spec the ticket.

## Fast pictures (Linux)

- `--headless` renders nothing. Plain `xvfb-run` works but Vulkan falls back to llvmpipe on the CPU (Xvfb has no
  DRI3): measured on an RX 7900 XTX, 20 s of game time took 49.6 s on llvmpipe and 10.3 s on the GPU.
- On the GPU, no window: `MESA_VK_WSI_DEBUG=sw xvfb-run -a -s '-screen 0 1920x1080x24' <godot> --gpu-index 0 ...`
  (Mesa presents through a CPU copy). Check the log says the real device, not `llvmpipe`.
- Goldens belong to one renderer: the two differ by about 0.8% RMSE. Make and compare goldens on the same one.
- Budget: iterate with printed numbers; render once per change (the gate's views plus one before/after pair). A
  render over 2 minutes is simulating too much game time: warp, then wait a few frames.
- The flatpak Godot fails from a sandboxed agent shell with `bwrap: Can't find source path /run/user/<uid>/doc/...`:
  add `--no-documents-portal` to `flatpak run`.
