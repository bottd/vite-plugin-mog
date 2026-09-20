import 'virtual:mog-arborium.css';
import type { TocEntry } from 'vite-plugin-mog';

const links = [
  { href: '/index.html', label: 'Home' },
  { href: '/embeds.html', label: 'Embeds' },
];

// Metadata is whatever KDL the document declared, so it arrives untyped and has
// to be narrowed field by field.
function text(metadata: Record<string, unknown>, key: string): string | undefined {
  const value = metadata[key];
  return typeof value === 'string' ? value : undefined;
}

export function render(html: string, toc: TocEntry[], metadata: Record<string, unknown>) {
  const nav = document.getElementById('nav') as HTMLElement;
  const content = document.getElementById('content') as HTMLElement;

  // html mode exports metadata like every other mode, but nothing renders it for
  // you, so the page puts it in the head itself.
  const title = text(metadata, 'title');
  const description = text(metadata, 'description');
  if (title) document.title = title;
  if (description) {
    const meta = document.createElement('meta');
    meta.name = 'description';
    meta.content = description;
    document.head.appendChild(meta);
  }

  nav.innerHTML = `
    <span class="logo">vite-plugin-mog</span>
    ${links.map(({ href, label }) => `<a href="${href}">${label}</a>`).join('')}
  `;

  const contents = toc
    .map(
      entry => `<li style="margin-left:${(entry.level - 1) * 0.75}rem">
      <a href="#${entry.id}">${entry.title}</a>
    </li>`
    )
    .join('');

  content.innerHTML = `
    <aside class="toc"><ul>${contents}</ul></aside>
    <article>${html}</article>
  `;
}

// html mode hands back a string, so the page owns its own chrome.
const style = document.createElement('style');
style.textContent = `
  body {
    margin: 0 auto;
    max-width: 44rem;
    padding: 0 1.5rem 4rem;
    font-family: system-ui, sans-serif;
    color: #1a1a1a;
    background: #fafafa;
  }
  nav {
    display: flex;
    gap: 1rem;
    align-items: center;
    padding: 1rem 0;
    border-bottom: 1px solid #e5e5e5;
  }
  .logo { font-weight: 700; margin-right: auto; }
  nav a { color: #666; text-decoration: none; }
  .toc {
    margin: 1.5rem 0;
    padding: 0.5rem 1rem;
    background: #f0f0f0;
    border-radius: 6px;
  }
  .toc ul { list-style: none; margin: 0.5rem 0; padding: 0; }
  .toc a { color: #444; font-size: 0.9rem; }
  pre { padding: 1rem; border-radius: 6px; overflow-x: auto; }
`;
document.head.appendChild(style);
