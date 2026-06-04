/**
 * Plan defaults — pure factories for the builder's `ServicePlan` shape.
 *
 * Lives in a sibling module (not ServicePlanForm.tsx) so the form file only
 * exports its component — that keeps React Fast Refresh working and lets these
 * factories be imported (e.g. BuilderPage seeds its initial state from
 * `emptyPlan`) without dragging the component along.
 */

import type { ServicePlan, SetlistItem, SetlistItemKind } from "@/lib/bindings";

/** A fresh, empty item — the default a new row starts from. */
export function emptyItem(kind: SetlistItemKind = "welcome"): SetlistItem {
  return {
    kind,
    title: null,
    body: null,
    leader: null,
    time: null,
    copyright: null,
    page_break: false,
    song: null,
    scripture: null,
    asset: null,
  };
}

/** An empty plan with a single welcome row to start from. */
export function emptyPlan(): ServicePlan {
  return { title: null, church: null, date: null, items: [emptyItem()] };
}

/** A representative plan so the user (and the e2e smoke) can try the pipeline
 *  without typing. Mirrors a typical Norwegian høymesse skeleton. */
export function samplePlan(): ServicePlan {
  return {
    title: "Høymesse",
    church: "Vår Frelsers menighet",
    date: "1. juni 2026",
    items: [
      { ...emptyItem("welcome"), title: "Velkommen", leader: "Liturg" },
      {
        ...emptyItem("song"),
        title: "Måne og sol",
        copyright: "© Det Norske Misjonsselskap",
      },
      {
        ...emptyItem("scripture"),
        title: "Første lesning",
        body: "Johannes 3:16–21",
      },
      { ...emptyItem("sermon"), title: "Preken", leader: "Sokneprest" },
      { ...emptyItem("prayer"), title: "Forbønn" },
      { ...emptyItem("benediction"), title: "Velsignelse" },
    ],
  };
}
