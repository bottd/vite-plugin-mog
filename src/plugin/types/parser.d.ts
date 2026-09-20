// The parser's own types come from `@parser`, generated from the Rust source.
// Only what the plugin re-exports publicly is restated here, so the published
// declarations never point consumers at the internal `@parser` alias.
export interface TocEntry {
  level: number;
  title: string;
  id: string;
}
