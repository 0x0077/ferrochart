/** The FerroChart mark (app/icon.svg): four planes around an open core, in the current text color. */
export function Logo({ className = '' }: { className?: string }) {
  return (
    <svg viewBox="0 0 640 640" aria-hidden="true" className={`fill-current ${className}`}>
      <path d="M64 64H422L357 287H285L64 230Z" />
      <path d="M466 64H576V400L368 352V304Z" />
      <path d="M64 280L285 309V359L177 576H64Z" />
      <path d="M304 369H367L576 447V576H231Z" />
    </svg>
  );
}
