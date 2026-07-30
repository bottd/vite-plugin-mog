import Document, { metadata, toc } from '../fixtures/basic.mg';
import metadataOnly from '../fixtures/basic.mg?metadata';

Document satisfies import('svelte').Component;
metadata satisfies Record<string, unknown>;
toc[0]?.title satisfies string | undefined;
metadataOnly.metadata satisfies Record<string, unknown>;
