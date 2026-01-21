import type { ScanProgress } from '../types';

interface ScanStatusProps {
  progress: ScanProgress;
}

export function ScanStatus({ progress }: ScanStatusProps) {
  const percentage = progress.total
    ? Math.round((progress.scanned / progress.total) * 100)
    : 0;

  const phaseLabels: Record<ScanProgress['phase'], string> = {
    counting: 'Counting files...',
    walking: 'Discovering files...',
    indexing: 'Indexing assets...',
    dependencies: 'Resolving dependencies...',
    thumbnails: 'Generating thumbnails...',
    complete: 'Complete',
    cancelled: 'Cancelled',
  };

  return (
    <div className="scan-status">
      <div className="loading-spinner" />
      <div className="text">
        {phaseLabels[progress.phase]}
        {progress.total && ` (${progress.scanned}/${progress.total})`}
      </div>
      {progress.total && (
        <div className="progress-bar" style={{ flex: 1, maxWidth: '200px' }}>
          <div className="fill" style={{ width: `${percentage}%` }} />
        </div>
      )}
      {progress.current_path && (
        <div className="text" style={{ maxWidth: '300px', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {progress.current_path.split(/[\\/]/).pop()}
        </div>
      )}
    </div>
  );
}
