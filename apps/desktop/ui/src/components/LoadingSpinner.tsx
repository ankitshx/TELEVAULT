interface LoadingSpinnerProps {
  message?: string;
}

export function LoadingSpinner({
  message = "Loading data from in-process core...",
}: LoadingSpinnerProps) {
  return (
    <div className="spinner-container">
      <div className="spinner" />
      <span className="spinner-message">{message}</span>
    </div>
  );
}
