import document, { html, metadata, toc } from '../fixtures/basic.mg';
import metadataOnly from '../fixtures/basic.mg?metadata';

html satisfies string;
metadata satisfies Record<string, unknown>;
toc[0]?.title satisfies string | undefined;
document.html satisfies string;
metadataOnly.metadata satisfies Record<string, unknown>;
