2022-01-08 Add Store and delete and return Not Implemented.
           Been using the following page to recoup info on DICOMWeb:
           https://learn.microsoft.com/en-us/azure/healthcare-apis/dicom/dicom-services-conformance-statement
2022-01-07 Tried to deal with temporary accept header limitation for WADO. Only JSON support for now.
           Can't make a filter work to early exit if header is incorrect.
2022-01-07 minimal QIDO queries support
2022-12-21 extract data for the includefields from the DICOM files
2022-12-18 Improve help messages of various executables
           host and port default values now set by structopt
2022-12-17 In order to build an actual statically linked binary, need to set .cargo/config.toml with
           particular options. The problem is that it is used whatever the build profile.
           (TODO) Need to find a way to enable this only on release profile.
           Could remove the config.toml and use the options on the command line when building release:
           `RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --bin serve --target x86_64-unknown-linux-gnu`
2022-12-12 (in progress) Working on serve.rs::get_studies to add to the response the include
           fields that are not present in the index.
           To test:
           ```
           curl -s http://localhost:8080/studies?includefield=ImagePatientOrientation
           curl -s http://localhost:8080/studies?includefield=ImagePatientOrientation,PatientName,Toto
           ```
           (TODO) `warp` uses `serde-urlencoded` to parse query string and the following pattern:
           ```
           curl -s http://localhost:8080/studies?includefield=ImagePatientOrientation\&includefield=PatientName\&includefield=Toto
           ```
           is not handled. See https://github.com/seanmonstar/warp/issues/733#issuecomment-722359432 which then leads to
           https://github.com/nox/serde_urlencoded/issues/52#issuecomment-483138575 and
           https://github.com/nox/serde_urlencoded/issues/75#issuecomment-648257888.
           It would necessitate a non-neglibible amount of work to manage `includefield` the way the DICOM standard expects to:
           https://dicom.nema.org/dicom/2013/output/chtml/part18/sect_6.7.html#sect_6.7.1.1
           To create the DB:
           `cargo run --bin scan -- --config config.yaml --input-path /media/jedi/slowdisk/DICOM/ --sql-output slowdisk.db`
           To start the server:
           `cargo run --bin serve -- --sqlfile slowdisk.db`
