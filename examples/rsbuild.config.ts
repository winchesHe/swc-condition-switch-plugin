import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

export default defineConfig({
  plugins: [pluginReact()],
  tools: {
    swc: {
      jsc: {
        experimental: {
          plugins: [
            [
              './swc_condition_plugin.wasm',
              {}
            ]
          ]
        }
      }
    }
  }
});
