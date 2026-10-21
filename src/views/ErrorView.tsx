// Failures and cancellations (SPECS §5): plain message, per-file details for batches, Retry / Back.
import { useState } from "react";
import { motion } from "framer-motion";
import { AlertTriangle } from "lucide-react";
import { Button } from "../components/Button";
import { truncateMiddle } from "../lib/format";
import { revealOutput } from "../lib/ipc";
import type { JobDone } from "../lib/types";

interface Props {
  done: JobDone;
  onRetry: () => void;
  onBack: () => void;
}

const files = (n: number) => `${n} ${n === 1 ? "file" : "files"}`;

export function ErrorView({ done, onRetry, onBack }: Props) {
  const [details, setDetails] = useState(false);
  const saved = done.outputs.length;
  const failed = done.failures.length;

  let title: string;
  let message: string;
  if (done.cancelled) {
    title = "Cancelled";
    message = saved > 0 ? `${files(saved)} saved before cancelling.` : "Nothing was saved.";
  } else if (failed === 1 && saved === 0) {
    title = "Couldn't convert";
    message = done.failures[0].error.message;
  } else {
    title = `${saved} done, ${failed} failed`;
    message = "The other files were saved.";
  }
  const showList = failed > 1 || (failed === 1 && saved > 0);

  return (
    <motion.div
      className="flex flex-col gap-3 px-4 pb-3"
      animate={done.cancelled ? undefined : { x: [0, -6, 6, -4, 4, 0] }}
      transition={{ duration: 0.3 }}
      role="alert"
    >
      <div className="flex items-start gap-3">
        <AlertTriangle
          size={22}
          strokeWidth={1.75}
          className={`mt-0.5 shrink-0 ${done.cancelled ? "text-tertiary" : "text-error"}`}
        />
        <div className="min-w-0">
          <h2 className="type-title text-primary">{title}</h2>
          <p className="type-caption mt-0.5 text-secondary">
            {failed === 1 && saved === 0 && !done.cancelled && (
              <span className="text-primary">{truncateMiddle(done.failures[0].name, 32)}: </span>
            )}
            {message}
          </p>
        </div>
      </div>

      {showList && (
        <div>
          <button type="button" className="type-caption text-link" onClick={() => setDetails((d) => !d)}>
            {details ? "Hide details" : "Show details"}
          </button>
          {details && (
            <ul className="type-caption mt-1 max-h-32 overflow-y-auto text-secondary">
              {done.failures.map((f) => (
                <li key={f.name} className="truncate py-0.5">
                  <span className="text-primary">{truncateMiddle(f.name, 28)}</span> — {f.error.message}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      <div className="flex gap-2">
        <Button className="flex-1" onClick={onBack}>
          Back
        </Button>
        {saved > 0 && (
          <Button className="flex-1" onClick={() => void revealOutput(done.outputs[0].path).catch(() => {})}>
            Show in folder
          </Button>
        )}
        {!done.cancelled && (
          <Button variant="primary" className="flex-1" onClick={onRetry} autoFocus>
            Retry
          </Button>
        )}
      </div>
    </motion.div>
  );
}
