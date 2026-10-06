"use client";

import SwipeToast from "@/components/SwipeToast";
import { dismissToast, useToasts } from "@/lib/toast";

// Fuse colour by tone: green = done, amber = needs a person (a limit is near), teal = neutral info.
const FUSE = { ok: "var(--ok)", review: "var(--review)", info: "var(--burgundy)" } as const;

/** Bottom-right stack for toast(). Inline toasts in a fixed column, so each one folds away when it leaves. */
export function Toaster() {
  const toasts = useToasts();
  return (
    <div className="toaster pointer-events-none fixed bottom-6 right-6 z-[90] flex w-[min(360px,calc(100vw-32px))] flex-col items-end max-sm:bottom-4 max-sm:right-4">
      {toasts.map((t) => (
        <div key={t.id} className="pointer-events-auto w-full">
          <SwipeToast
            inline
            title={t.title}
            description={t.description}
            icon={t.icon}
            actionLabel={t.actionLabel}
            onAction={t.onAction}
            duration={t.duration ?? 4500}
            background="#0b1215" /* toasts stay dark in both themes */
            color="#f5f5f5"
            fuseColor={FUSE[t.tone ?? "info"]}
            width={360}
            radius={14}
            onClose={() => dismissToast(t.id)}
          />
        </div>
      ))}
    </div>
  );
}
