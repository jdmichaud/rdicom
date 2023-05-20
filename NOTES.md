2023-05-19 Abstract the access to bulk data so that we can work with a filesystem
           in memory the same way we are able to create the sqlite DB in 
           memory using :memory:. This way, when executing tests, we can launch
           the server entirely in memory with no side effect to the file system.
           To implement this we have a InstanceFactory trait that provide an
           instance based on a key. For the file system the implementation will
           just open the path and for the memory the path will be used as a key
           to a hash map. Need to assess performance impact.
           In addition, the path to the DICOM file is not relative in the base
           and the server must be provided with the root to the DICOM files.
2023-04-14 Been working on various format translation between binary DICOM, json
           and xml.
           This lead to the creation of `dicom_representation.rs` which contains
           the data structure to represent xml and json.
           Unfortunately, DICOM defines the xml and json format differently. As
           a consequence we need two slightly different data structure depending
           on whether we coming from or going to json or xml.
           The differences are mainly:
           1. json will represent a DICOM instance as an object which keys are
           the DICOM tags (xxxx,yyyy) and the values are the attribute content.
           Whereas xml will treat a DICOM instance as an array where each entry
           contains its DICOM tag as a field in the dicom attribute. As a
           consequence, a DicomAttribute node can not be represented the same way
           for json and xml, that is why two object exists: DicomAttribute and
           DicomAttributeJson.
           2. json represents sequences as subobject inside the Value field of a
           dicom attribute whereas xml represents sequences as a particular tag
           (Item) inside the attribute directly. For this reason, the enum entry
           `Payload::Item` is used to represent sequences in xml and the enum
           entry `ValuePayload::Sequence` is used for json.
           Both representation are not compatible with each other even though it
           is always possible to generate json or xml from both structures but
           they would not be valid according to the DICOM standard.
           As a consequence of these discrepencies, translation functions are
           necessary to go from one representation to another.
2023-01-08 Add Store and delete and return Not Implemented.
           Been using the following page to recoup info on DICOMWeb:
           https://learn.microsoft.com/en-us/azure/healthcare-apis/dicom/dicom-services-conformance-statement
2023-01-07 Tried to deal with temporary accept header limitation for WADO. Only JSON support for now.
           Can't make a filter work to early exit if header is incorrect.
2023-01-07 minimal QIDO queries support
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
