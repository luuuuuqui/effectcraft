# Context

This [fork](https://github.com/luuuuuqui/effectcraft) starts from the
[original project](https://github.com/storytold/effectcraft) and adapts `.aep` / `.aepx` import
from [glumbis's fork](https://github.com/glumbis/effectcraft).

The original changes are commits
[340b5c6b](https://github.com/glumbis/effectcraft/commit/340b5c6b2beaa727166a82fb311265b10470c0a9),
[60ec5578](https://github.com/glumbis/effectcraft/commit/60ec5578a04eba8a82a6062f8f865dd8786402eb) and
[088ca4dd](https://github.com/glumbis/effectcraft/commit/088ca4dd80f259886f842a1083ef0b52686f99fb),
by glumbis and contributors, licensed under MIT OR Apache-2.0.

This fork also fixes GPU rounding at Dust & Scratches and Posterize thresholds and CC Grid Wipe
tile boundaries.

# Goal

Import existing projects through **File ▸ Import ▸ After Effects Project...** or `file.importAep`.
The external converter uses the MIT-licensed `py-aep` package and generates a script that rebuilds
the project through EffectCraft's scripting API. The main composition opens after import.

# Constraints

- The feature requires an `aep` build, Python 3 and `py-aep==0.18.1`.
- Import runs in native builds. The web build does not execute external processes.
- OpenFX is excluded from this integration. The workspace's prohibition on `unsafe` remains.
- Third-party effects, layer styles and other unsupported features are listed in the import report.
  See the [converter's limitations](tools/aep-import/README.md).
- Large or malformed files can fail conversion. The app reports the error and preserves the open
  document when conversion fails. Generated scripts are limited to 32 MiB, diagnostics to 64 KiB.

# Examples

On Linux or macOS, prepare an isolated Python environment:

```sh
python3 -m venv .venv-aep
.venv-aep/bin/python -m pip install -r tools/aep-import/requirements.txt
EFFECTCRAFT_PYTHON="$PWD/.venv-aep/bin/python" cargo run --release -p effectcraft --features aep
```

Import from the command line:

```sh
EFFECTCRAFT_PYTHON="$PWD/.venv-aep/bin/python" cargo run --release -p effectcraft-cli --features aep -- \
  exec file.importAep '{"path":"project.aep"}' --empty --save-as project.ecproj
```

Check the converter and host integration:

```sh
.venv-aep/bin/python -m unittest discover -s tools/aep-import -v
EFFECTCRAFT_PYTHON="$PWD/.venv-aep/bin/python" cargo test --release -p effectcraft-host --features aep --test aep
```

The integration test generates its own `.aep` file with an animated layer and markers.
No binary fixtures or Adobe-authored projects are included in the repository.
