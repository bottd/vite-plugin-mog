import document, { metadata, toc } from '../fixtures/basic.mg';
import metadataOnly from '../fixtures/basic.mg?metadata';

metadata satisfies Record<string, unknown>;
toc[0]?.id satisfies string | undefined;
document.metadata satisfies Record<string, unknown>;
metadataOnly.toc[0]?.title satisfies string | undefined;
