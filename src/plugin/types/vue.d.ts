/* eslint-disable @typescript-eslint/triple-slash-reference */
/// <reference path="./metadata-query.d.ts" />

declare module '*.mg' {
  type MogTocEntry = import('./parser.js').TocEntry;

  export const metadata: Record<string, unknown>;
  export const toc: MogTocEntry[];
  const component: import('vue').DefineComponent<object, object, unknown>;
  export default component;
}
