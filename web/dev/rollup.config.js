// rollup.config.js
import sourcemaps from 'rollup-plugin-sourcemaps';
import typescript from 'rollup-plugin-typescript2';
import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import serve from 'rollup-plugin-serve';
import copy from 'rollup-plugin-copy';

const isWatch = process.env.ROLLUP_WATCH;

export default {
  input: 'dev/main.ts',
  output: {
    name: 'Dev',
    file: 'dev/dist/main.js',
    format: 'umd',
    sourcemap: true,
  },
  watch: {
    include: ['*.ts', 'index.html'],
    chokidar: {
      usePolling: true
    }
  },
  plugins: [
    resolve(),
    commonjs(),
    sourcemaps(),
    typescript(),
    copy({
      targets: [
        { src: 'dev/index.html', dest: 'dev/dist/' },
        { src: '../target/wasm32-unknown-unknown/debug/rdicom.wasm', dest: 'dev/dist/' },
      ],
      verbose: true,
      copyOnce: true,
    }),
    isWatch && serve('dev/dist'),
  ],
}
