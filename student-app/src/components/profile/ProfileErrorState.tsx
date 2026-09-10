import { TriangleAlert } from 'lucide-react';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';

interface Props {
  message?: string;
  onRetry: () => void;
}

/**
 * Clean error state for a profile that could not be loaded, with a Retry that
 * re-issues the request.
 */
export function ProfileErrorState({ message, onRetry }: Props) {
  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-6 sm:px-6 lg:px-8">
      <Card variant="solid" padding="lg" className="mx-auto max-w-md text-center">
        <span
          className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-red-50 text-red-600"
          aria-hidden
        >
          <TriangleAlert size={22} />
        </span>
        <h2 className="mt-4 text-lg font-semibold text-ink-900">Unable to load your profile</h2>
        <p className="mt-2 text-sm text-ink-500">
          {message ?? 'Something went wrong while retrieving your information.'} Your session is safe — please
          try again.
        </p>
        <Button className="mt-6" onClick={onRetry}>
          Retry
        </Button>
      </Card>
    </div>
  );
}
