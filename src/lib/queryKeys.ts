/**
 * Centralised TanStack Query keys.
 *
 * These keys drive cross-component cache invalidation: BuilderPage creates a
 * project and invalidates the project list that DocumentSelector / ExportPage /
 * ProjectsPanel read; FormsPage invalidates a project's documents that
 * DocumentSelector lists. If a key literal drifts in one place, invalidation
 * silently breaks. Define them ONCE here and import everywhere.
 */

/** Query key for the project list. */
export const projectsKey = ["projects"] as const;

/** Query key for a project's documents. */
export const documentsKey = (projectId: string) =>
  ["documents", projectId] as const;

/** Query key for the song catalog list. */
export const songsKey = ["songs"] as const;

/** Query key for the document-template list. */
export const docTemplatesKey = ["docTemplates"] as const;
