import Document, { Component, metadata, toc } from '../fixtures/basic.mg';
import metadataOnly from '../fixtures/basic.mg?metadata';

Document satisfies import('react').ComponentType;
Component satisfies import('react').ComponentType;
metadata satisfies Record<string, unknown>;
toc[0]?.id satisfies string | undefined;
metadataOnly.toc[0]?.level satisfies number | undefined;
