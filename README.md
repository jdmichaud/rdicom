# rdicom

`ridcom` is set of tools for DICOM written in rust. It contains:
- A rust library for parsing DICOM files.
- `scan` a indexing tool to recursively parse a set of DICOM files and generate an index (sqlite or csv).
- `serve` a dicom-web server based on the index generated.

⚠️ `rdicom` is not ready for production and only partially implement the DICOM
file format and the dicom-web API. Moreover its implementation is far from being
optimised.

## `serve`

`serve` will serve a DICOMWeb service backed by a sqlite database previously created
by `scan`.

How to start `serve`?:
```bash
serve --sqlfile base.db
```

or from cargo:
```bash
cargo run --bin serve -- --sqlfile base.db
```

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