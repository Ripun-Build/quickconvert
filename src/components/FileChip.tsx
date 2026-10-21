// Header chip for the selected files (DESIGN §4): one thumbnail + name, or a fan of three + count.
import { describeCount, formatBytes, totalSize, truncateMiddle } from "../lib/format";
import type { FileSet } from "../lib/types";
import { Thumbnail } from "./Thumbnail";

const FAN = ["-rotate-4 translate-x-0", "rotate-0 translate-x-1", "rotate-4 translate-x-2"];

export function FileChip({ set }: { set: FileSet }) {
  const { files, unsupported } = set;
  const single = files.length === 1;
  const fan = files.slice(0, 3);
  const skipped = unsupported > 0 && (
    <span className="text-caution">
      {" · "}
      {unsupported} {unsupported === 1 ? "file" : "files"} not supported
    </span>
  );

  return (
    <div className="flex min-w-0 items-center gap-3">
      {single ? (
        <Thumbnail file={files[0]} />
      ) : (
        <div className="relative h-8 w-10 shrink-0">
          {fan.map((file, i) => (
            <Thumbnail
              key={file.path}
              file={file}
              className={`absolute top-0 left-0 ${FAN[fan.length === 1 ? 1 : i]}`}
            />
          ))}
        </div>
      )}
      <div className="min-w-0">
        <p className="type-body-strong truncate text-primary" title={single ? files[0].name : undefined}>
          {single ? truncateMiddle(files[0].name) : describeCount(files)}
        </p>
        <p className="type-caption truncate text-secondary tabular-nums">
          {formatBytes(totalSize(files))}
          {skipped}
        </p>
      </div>
    </div>
  );
}
