import { chromium, type Browser } from 'playwright';
import { parseMog, DataAttributesMode } from '../../dist/napi/index.js';

// R9 emits a `data-*` value single-quoted when that costs less. Both spellings
// have to reach `dataset` as the same string — which only a real HTML parser
// can settle.
let browser: Browser;

beforeAll(async () => {
  browser = await chromium.launch();
});
afterAll(async () => {
  await browser?.close();
});

async function dataset(source: string): Promise<{ html: string; impact: unknown }> {
  const result = await parseMog(source, 'html', { mode: DataAttributesMode.all, keys: undefined });
  const html = result.segments.map(segment => ('html' in segment ? segment.html : '')).join('');

  const page = await browser.newPage();
  try {
    await page.setContent(html);
    const raw = await page
      .locator('[data-impact]')
      .first()
      .evaluate(el => el.dataset.impact);
    return { html, impact: JSON.parse(raw as string) };
  } finally {
    await page.close();
  }
}

it('round-trips a JSON value through a single-quoted attribute', async () => {
  const { html, impact } = await dataset(
    '=card:\n``attr:\nimpact { all before=0.505 }\n``\n# A\n=\n'
  );

  expect(html).toContain("data-impact='");
  expect(impact).toEqual({ all: { before: 0.505 } });
});

it('round-trips a value holding both quote characters', async () => {
  const value = `say "hi" o'clock`;
  const { impact } = await dataset(
    `=card:\n\`\`attr:\nimpact "${JSON.stringify(value).slice(1, -1)}"\n\`\`\n# A\n=\n`
  );

  expect(impact).toBe(value);
});

it('round-trips a value that stays double-quoted', async () => {
  const { html, impact } = await dataset('=card:\n``attr:\nimpact "it\'s o\'clock"\n``\n# A\n=\n');

  expect(html).toContain('data-impact="');
  expect(impact).toBe("it's o'clock");
});
