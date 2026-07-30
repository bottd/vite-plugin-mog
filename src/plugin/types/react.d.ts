/* eslint-disable @typescript-eslint/triple-slash-reference */
/// <reference path="./metadata-query.d.ts" />

declare module '*.mg' {
  type MogTocEntry = import('./parser.js').TocEntry;
  type MogComponent = import('react').ComponentType;

  export const metadata: Record<string, unknown>;
  export const toc: MogTocEntry[];
  export const Component: MogComponent;
  const _default: MogComponent;
  export default _default;
}
