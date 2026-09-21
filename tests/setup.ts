import { vi } from 'vitest';

vi.mock('@parser', async () => {
  const mod = await import('../dist/napi/index.js');
  return {
    parseMog: mod.parseMog,
    parseMogMetadata: mod.parseMogMetadata,
    getThemeCss: mod.getThemeCss,
    themeNames: mod.themeNames,
    OutputMode: mod.OutputMode,
    DataAttributesMode: mod.DataAttributesMode,
  };
});
