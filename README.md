# rdicom

## Contributes

In order to build rdicom in webassembly, ensure the wasi target is installed locally:
```bash
rustup target add wasm32-wasi
```

Then build with:
```bash
cargo build --target wasm32-wasi
```

The library `crate-type` must be set to `cdylib` in `Cargo.toml`.

## `data-element.csv`

`data-element.csv` is generated in the `dicom-model` project.

From this file to can create the `dicom-tags` with `generate-dicom-tags.sh`:
```bash
./generate-dicom-tags.sh data-elements.csv > src/dicom_tags.rs
```