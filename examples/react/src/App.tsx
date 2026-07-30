import { useState } from 'react';
import 'virtual:mog-arborium.css';
import Index, { metadata as indexMeta } from '../content/index.mg';
import Embeds, { metadata as embedsMeta } from '../content/embeds.mg';

const pages = [
  { id: 'index', Component: Index, meta: indexMeta },
  { id: 'embeds', Component: Embeds, meta: embedsMeta },
];

export function App() {
  const [currentId, setCurrentId] = useState('index');
  const current = pages.find(page => page.id === currentId) ?? pages[0];

  return (
    <>
      <nav>
        <span className="logo">vite-plugin-mog</span>
        {pages.map(({ id, meta }) => (
          <button
            key={id}
            className={currentId === id ? 'active' : ''}
            onClick={() => setCurrentId(id)}
          >
            {meta.title}
          </button>
        ))}
      </nav>

      <main>
        <current.Component />
      </main>

      <style>{`
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
        nav button { border: none; background: none; color: #666; font-size: 0.95rem; cursor: pointer; }
        nav button.active { color: #1a1a1a; font-weight: 600; }
        main pre { padding: 1rem; border-radius: 6px; overflow-x: auto; }
      `}</style>
    </>
  );
}
