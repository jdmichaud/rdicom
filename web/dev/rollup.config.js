// rollup.config.js
import sourcemaps from 'rollup-plugin-sourcemaps';
import typescript from 'rollup-plugin-typescript2';
import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import serve from 'rollup-plugin-serve';
import copy from 'rollup-plugin-copy';

const isWatch = process.env.ROLLUP_WATCH;

export default [{
  input: 'dev/load-one-file.ts',
  output: {
    name: 'Dev',
    file: 'dev/dist/load-one-file.js',
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
        { src: 'dev/load-one-file.html', dest: 'dev/dist/' },
      ],
      verbose: true,
      copyOnce: true,
    }),
    isWatch && serve('dev/dist'),
  ],
}, {
  input: 'dev/split.ts',
  output: {
    name: 'Dev',
    file: 'dev/dist/split.js',
    format: 'umd',
    sourcemap: true,
  },
  watch: {
    include: ['*.ts', 'split.html'],
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
        { src: 'dev/split.html', dest: 'dev/dist/' },
      ],
      verbose: true,
      copyOnce: true,
    }),
    isWatch && serve('dev/dist'),
  ],
}]
