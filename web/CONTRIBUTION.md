# General overview

The rdicom Web package provides typescript classes to parse and access DICOM
data using a version of the rdicom library compiled to wasm.

The library is composed of generated classes (base on
[dicom-model](https://www.npmjs.com/package/@jdmichaud/dicom-model)) and utility
functions to interact with the wasm bundle generated from rdicom rust code.

It also comes with a dev app (`dev/`) to test out changes.

# Typical development workflow

## Generate classes

You might change the generated class template or update the generation script,
then you will need to generate the classes anew:
```bash
npm run generate-classes
```
See `package.json` `scripts` field for more detail on how generating classes
work.

## Build the module

After development, you will need to build the module:
```bash
npm run build
```
This basically runs rollup on its config. It generates the module as a minified
and non minified version along with their sourcemaps.

For a shorter development cycle, you might want to watch changes:
```bash
npx rollup --watch --config rollup.config.ts
```

## Test with the development application

The development application is present in `dev/`. It consumes the module
(generated in `dist/`) and provide a webpage were you can choose several dev app
testing various aspect of the module.

To start the dev app:
```bash
npm start
```

This will watch the source file and the rdicom module in `dist/`.

You can then open your browser to http://localhost:10001.

When initializing the `LocalDicomInstanceDecoder` you might want to call the
initialization function (`init`) with the path to the debug version of the
rdicom library:
```typescript
  await instanceDecoder.init('rdicom.debug.wasm');
```
Make sure that you have a symbolic link in your `dist` folder to the debug
version of the library:
```bash
cd dist
ln -s ../../../target/wasm32-unknown-unknown/debug/rdicom.wasm rdicom.debug.wasm
```

## Publishing

Before publishing, make sure that you will distribute the latest version of the
`rdicom.wasm` binary.
