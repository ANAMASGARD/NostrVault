export function ArchivePanel() {
  return (
    <section aria-labelledby="archive-title">
      <h2 id="archive-title">Portable snapshot</h2>
      <p>
        Exports use a versioned authenticated envelope named
        nostrvault-date-snapshot.nvarchive. A wrong password, truncated file, or
        unsafe path is rejected and the previous vault snapshot stays in place.
        This screen does not upload the archive. A downloaded file is a snapshot
        from that moment. Closing the browser does not keep relay catch-up
        running.
      </p>
    </section>
  );
}
