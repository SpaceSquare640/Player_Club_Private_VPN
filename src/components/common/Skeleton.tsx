/** Generic skeleton-screen placeholder shown while a route chunk loads. */
export function Skeleton() {
  return (
    <div className="space-y-6" data-testid="skeleton" aria-busy="true">
      <div className="h-8 w-48 animate-pulse rounded-lg bg-white/5" />
      <div className="grid grid-cols-[1.618fr_1fr] gap-6">
        <div className="h-40 animate-pulse rounded-2xl bg-white/5" />
        <div className="h-40 animate-pulse rounded-2xl bg-white/5" />
      </div>
    </div>
  );
}
