export interface EmbedComponent {
  index: number;
  mode: string;
  code: string;
}

export interface TocEntry {
  level: number;
  title: string;
  id: string;
}

export interface MogParseResult {
  metadata: Record<string, unknown>;
  htmlParts: string[];
  toc: TocEntry[];
  embedComponents: EmbedComponent[];
  embedCss: string;
  diagnostics?: string[];
}
