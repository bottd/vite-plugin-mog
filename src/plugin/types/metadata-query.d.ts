declare module '*.mg?metadata' {
  type MogTocEntry = import('./parser.js').TocEntry;

  export const metadata: Record<string, unknown>;
  export const toc: MogTocEntry[];
  const _default: {
    metadata: Record<string, unknown>;
    toc: MogTocEntry[];
  };
  export default _default;
}
