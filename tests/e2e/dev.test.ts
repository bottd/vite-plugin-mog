import { join } from 'node:path';
import { chromium } from 'playwright';
import { createLogger, createServer } from 'vite';

const examplesDir = join(import.meta.dirname, '../../examples');

async function openExample(example: string, path: string) {
  // A dep-scan failure only reaches the browser as missing content, so collect
  // what the server says too.
  const errors: string[] = [];
  const record = (message: string) => errors.push(message);
  const server = await createServer({
    root: join(examplesDir, example),
    logLevel: 'silent',
    customLogger: {
      ...createLogger('silent'),
      warn: record,
      warnOnce: record,
      error: record,
    },
    server: { port: 0 },
    // Without this a warm node_modules/.vite skips the dep scan, and with it the
    // scan-related assertions below.
    optimizeDeps: { force: true },
  });
  const browser = await chromium.launch();
  const page = await browser.newPage();
  page.on('console', message => {
    if (message.type() === 'error') errors.push(message.text());
  });
  page.on('pageerror', error => errors.push(error.message));

  await server.listen();
  const [url] = server.resolvedUrls?.local ?? [];
  await page.goto(new URL(path, url).href);

  return {
    page,
    errors,
    async close() {
      await browser.close();
      await server.close();
    },
  };
}

it.each([
  { example: 'svelte', heading: 'Mog in Svelte' },
  { example: 'vue', heading: 'Mog in Vue' },
  { example: 'react', heading: 'Mog in React' },
])('renders and hydrates the $example example in dev', async ({ example, heading }) => {
  const { page, errors, close } = await openExample(example, '.');
  try {
    await page.getByRole('heading', { name: heading }).waitFor();
    // A highlighted code block from index.mg, styled by the theme CSS.
    await page.locator('pre.arborium .line').first().waitFor();

    await page.getByRole('button', { name: 'Embeds' }).click();
    // The embed is a real framework component, so it holds state across a click.
    const counter = page.getByRole('button', { name: /Clicked/ });
    await counter.click();
    expect(await counter.textContent()).toBe('Clicked 1 times');
    // embed:css reaches the page as a stylesheet, not as inline text.
    const note = page.locator('.mog-note');
    await note.waitFor();
    expect(await note.evaluate(el => getComputedStyle(el).borderLeftColor)).toBe(
      'rgb(102, 51, 153)'
    );

    expect(errors).toEqual([]);
  } finally {
    await close();
  }
});

it('renders the html example in dev', async () => {
  const { page, errors, close } = await openExample('html', 'embeds.html');
  try {
    await page.getByRole('heading', { name: 'Embeds' }).waitFor();
    expect(await page.locator('.mog-note').textContent()).toContain('Inline markup');
    expect(errors).toEqual([]);
  } finally {
    await close();
  }
});
