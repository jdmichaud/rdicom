# rdicom-web

Provides DICOM access in Javascript. This packages relies on the [rdicom](https://github.com/jdmichaud/rdicom)
library to parse DICOM files.

### Usage

Install the package using npm:
```bash
npm install --save @jdmichaud/rdicom-web
```

To load a DICOM file
```typescript
import { LocalDataset, LocalDicomInstanceDecoder } from '../dist/index';

const instanceDecoder = new LocalDicomInstanceDecoder();
await instanceDecoder.init();
// We assume the file content is in a ArrayBuffer called buffer
const instance = instanceDecoder.getInstanceFromBuffer(buffer);
const columns = localDataset.getColumns();
const rows = localDataset.getRows();
console.log(`columns ${columns} rows ${rows}`);
```
