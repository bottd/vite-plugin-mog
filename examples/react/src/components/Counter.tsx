import { useState } from 'react';

export default function Counter() {
  const [count, setCount] = useState(0);
  return (
    <button
      style={{
        padding: '0.4rem 0.9rem',
        border: '1px solid #d0d0d0',
        borderRadius: '6px',
        background: 'white',
        cursor: 'pointer',
      }}
      onClick={() => setCount(c => c + 1)}
    >
      Clicked {count} times
    </button>
  );
}
