import sourcemaps from 'rollup-plugin-sourcemaps';
import typescript from 'rollup-plugin-typescript2';
import terser from '@rollup/plugin-terser';
import copy from 'rollup-plugin-copy';
import arraybuffer from '@wemap/rollup-plugin-arraybuffer';

export default {
  input: 'src/index.ts',
  output: [{
    name: 'rdicom',
    file: 'dist/index.js',
    format: 'umd',
    sourcemap: true,
  }, {
    name: 'rdicom',
    file: 'dist/index.min.js',
    format: 'umd',
    plugins: [terser()],
  }],
  plugins: [
    arraybuffer({ include: '**/*.wasm' }),
    sourcemaps(),
    typescript(),
    copy({
      targets: [
        { src: '../target/wasm32-unknown-unknown/release/rdicom.wasm', dest: 'dist/' },
      ],
      verbose: true,
      copyOnce: true,
    }),
  ],
}
