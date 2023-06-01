# rdicom

`ridcom` is set of tools for DICOM written in rust. It contains:
- A rust library for parsing DICOM files.
- `dump` a simplified clone of [`dcmdump`](https://support.dcmtk.org/docs/dcmdump.html).
- `scan` an indexing tool to recursively parse a set of DICOM files and generate an index (sqlite or csv).
- `serve` a [dicomweb](https://www.dicomstandard.org/using/dicomweb) server based on the index generated.

⚠️ `rdicom` is not ready for production and only partially implement the DICOM
file format and the dicom-web API. Moreover its implementation is far from being
optimised.

## `dump`

`dump` will dump the content of a dicom file to your terminal using the same presentation as
dcmdump:
```bash
cargo run --bin dump -- /path/to/some/dicom/file
```

Once build, the binary can also be found in the target folder:
```bash
cargo build --bin dump
target/x86_64-unknown-linux-gnu/debug/dump /path/to/some/dicom/file
```

## `scan`

`scan` will recursively scan a folder for dicom files and extract dicom value based
on a provided configuration file. The fields are then either dump as
[CSV](https://en.wikipedia.org/wiki/Comma-separated_values)
to the standard output or into an [sqlite](https://sqlite.org/index.html) database.

```bash
./scan --config config.yaml --sql-output index.db /path/to/DICOM
```

The configuration will list the fields to be extracted to the index database. For example:
```yaml
indexing:
  fields:
    studies:
      - StudyInstanceUID
      - PatientName
      - PatientID
      - PatientBirthDate
      - AccessionNumber
      - ReferringPhysicianName
      - StudyDate
      - StudyDescription
    series:
      - SeriesInstanceUID
      - Modality
    instances:
      - SOPInstanceUID
table_name: dicom_index
```

## `serve`

`serve` will serve a DICOMWeb service backed by a sqlite database previously created
by `scan`.

How to start `serve`?:
```bash
serve --sqlfile base.db /path/to/DICOM
```

or from cargo:
```bash
cargo run --bin serve -- --sqlfile base.db /path/to/DICOM
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