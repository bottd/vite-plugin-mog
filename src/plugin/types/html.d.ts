/* eslint-disable @typescript-eslint/triple-slash-reference */
/// <reference path="./metadata-query.d.ts" />

declare module '*.mg' {
  type MogTocEntry = import('./parser.js').TocEntry;

  export const metadata: Record<string, unknown>;
  export const html: string;
  export const toc: MogTocEntry[];
  const _default: {
    metadata: Record<string, unknown>;
    html: string;
    toc: MogTocEntry[];
  };
  export default _default;
}
