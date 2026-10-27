// Convert to PDF for Word / PowerPoint / Excel. For each type of file, the ways to make the PDF as
// cards: Quick (built in, close but not identical), and exact copies made with Pages / Keynote /
// Numbers or LibreOffice — with Install buttons when they're missing, and an ⓘ panel saying what
// each one is, why it's better, which version, how big, and where it comes from.
import { useEffect, useMemo, useState } from "react";
import { Check, ChevronLeft, Info, LoaderCircle } from "lucide-react";
import { Button } from "../components/Button";
import { Segmented } from "../components/Segmented";
import {
  officeCancelInstall,
  officeEngines,
  officeInstallLibreOffice,
  officeInstallStatus,
  officeLibreOfficePackage,
  officeOpenStore,
  onOfficeInstall,
  saveSettings,
} from "../lib/ipc";
import {
  APPLE_APP,
  DETAILS,
  ENGINE_NAME,
  TYPE_LABEL,
  byType,
  chooseFor,
  enginesFor,
  fontsNote,
  megabytes,
  type DocType,
  type Engine,
  type EngineInfo,
} from "../lib/office";
import type { FileInfo, InstallProgress, JobRequest, LibreOfficePackage, Settings } from "../lib/types";

/** How often installed apps are checked again while this screen is open (App Store installs). */
const RECHECK_MS = 3000;

interface Props {
  files: FileInfo[];
  settings: Settings | null;
  onBack: () => void;
  /** `files`: only those that can be converted now. */
  onRun: (request: JobRequest, files: FileInfo[]) => void;
}

export function OfficeView({ files, settings, onBack, onRun }: Props) {
  const groups = useMemo(() => byType(files), [files]);
  const [infos, setInfos] = useState<EngineInfo[] | null>(null);
  const [picks, setPicks] = useState<Partial<Record<DocType, Engine | null>>>(() => ({ ...settings?.office }));
  const [focus, setFocus] = useState<{ engine: Engine; type: DocType } | null>(null);
  const [a4, setA4] = useState(true);
  const [landscape, setLandscape] = useState(false);
  const [firstSheetOnly, setFirstSheetOnly] = useState(false);
  const [install, setInstall] = useState<InstallProgress | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [storeOpened, setStoreOpened] = useState<Set<Engine>>(new Set());
  const [pkg, setPkg] = useState<LibreOfficePackage | null>(null);

  // Installed apps now, and again every few seconds (an App Store install shows up by itself).
  useEffect(() => {
    let live = true;
    const check = () =>
      officeEngines()
        .then((list) => live && setInfos(list))
        .catch(() => {});
    void check();
    const timer = setInterval(check, RECHECK_MS);
    officeLibreOfficePackage()
      .then((p) => live && setPkg(p))
      .catch(() => {});
    officeInstallStatus()
      .then((p) => live && p && setInstall(p))
      .catch(() => {});
    const unlisten = onOfficeInstall((p) => {
      setInstall(p);
      if (p.phase === "done") void check();
    });
    return () => {
      live = false;
      clearInterval(timer);
      void unlisten.then((fn) => fn());
    };
  }, []);

  const ready = infos ?? [];
  const chosen = Object.fromEntries(groups.map((g) => [g.type, chooseFor(g.files, picks[g.type], ready)])) as Partial<
    Record<DocType, Engine | null>
  >;
  const convertible = groups.filter((g) => chosen[g.type]).flatMap((g) => g.files);
  const blocked = groups.filter((g) => infos && !chosen[g.type]);
  const usesQuick = groups.some((g) => chosen[g.type] === "quick");
  const quickSheets = groups.some((g) => g.type === "sheet" && chosen[g.type] === "quick");
  const shown = focus ?? (groups[0] ? { engine: chosen[groups[0].type] ?? APPLE_APP[groups[0].type], type: groups[0].type } : null);
  const installing = install !== null && ["downloading", "checking", "installing"].includes(install.phase);

  const pick = (type: DocType, engine: Engine) => {
    setPicks((p) => ({ ...p, [type]: engine }));
    setFocus({ engine, type });
    // Remembered per type (Settings → Conversions shows it too).
    if (settings) void saveSettings({ ...settings, office: { ...settings.office, [type]: engine } }).catch(() => {});
  };

  const startInstall = () => {
    setConfirming(false);
    setInstall({ phase: "downloading", received: 0, total: pkg?.bytes ?? 0, message: null, path: null });
    void officeInstallLibreOffice().catch(() => {});
  };

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (convertible.length === 0) return;
    const engines: Partial<Record<DocType, Engine>> = {};
    for (const g of groups) {
      const engine = chosen[g.type];
      if (engine) engines[g.type] = engine;
    }
    onRun({ action: "office_to_pdf", engines, quick: { a4, landscape, firstSheetOnly } }, convertible);
  };

  const n = convertible.length;
  const label =
    !infos ? "Checking your apps…" : n === 0 ? "Install an app to convert these" : `Convert ${n} ${n === 1 ? "file" : "files"} to PDF`;

  return (
    <form onSubmit={submit} className="flex flex-col gap-3 px-4 pb-3">
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={onBack}
          aria-label="Back"
          className="-ml-1.5 flex size-7 items-center justify-center rounded-control text-secondary hover:bg-surface-hover"
        >
          <ChevronLeft size={18} strokeWidth={1.75} />
        </button>
        <h2 className="type-title text-primary">Convert to PDF</h2>
        <span className="type-caption ml-auto text-tertiary tabular-nums">
          {files.length} {files.length === 1 ? "file" : "files"}
        </span>
      </div>

      <div className="flex gap-4">
        <div className="scroll-fade flex h-[316px] min-w-0 flex-1 flex-col gap-4 overflow-y-auto py-1">
          {groups.map((group) => (
            <section key={group.type} className="flex flex-col gap-1.5">
              <span className="type-caption text-secondary">
                {TYPE_LABEL[group.type]} · {group.files.length} {group.files.length === 1 ? "file" : "files"}
              </span>
              <div role="radiogroup" aria-label={`How to make the ${TYPE_LABEL[group.type]} PDF`} className="flex flex-col gap-1">
                {enginesFor(group.files).map((engine) => (
                  <EngineCard
                    key={engine}
                    engine={engine}
                    type={group.type}
                    info={ready.find((i) => i.engine === engine)}
                    checking={!infos}
                    selected={chosen[group.type] === engine}
                    focused={shown?.engine === engine && shown.type === group.type}
                    onPick={() => pick(group.type, engine)}
                    onInfo={() => setFocus({ engine, type: group.type })}
                    onStore={() => {
                      setStoreOpened((s) => new Set(s).add(engine));
                      void officeOpenStore(engine).catch(() => {});
                    }}
                    storeOpened={storeOpened.has(engine)}
                    pkg={pkg}
                    install={engine === "libreoffice" ? install : null}
                    confirming={engine === "libreoffice" && confirming}
                    onAskInstall={() => {
                      setFocus({ engine, type: group.type });
                      setConfirming(true);
                    }}
                    onConfirm={startInstall}
                    onNotNow={() => setConfirming(false)}
                    onCancelInstall={() => void officeCancelInstall().catch(() => {})}
                    installing={installing}
                  />
                ))}
              </div>
              {infos && !chosen[group.type] && (
                <span className="type-caption text-caution">
                  {TYPE_LABEL[group.type]} files need an app to make a PDF. Install one above.
                </span>
              )}
            </section>
          ))}
        </div>

        <aside aria-label="Details" className="scroll-fade h-[316px] w-[244px] shrink-0 overflow-y-auto rounded-card bg-surface p-3">
          {shown && <DetailsPanel engine={shown.engine} type={shown.type} info={ready.find((i) => i.engine === shown.engine)} pkg={pkg} />}
        </aside>
      </div>

      {usesQuick && (
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <span className="type-caption text-secondary">Quick (built in):</span>
          <div className="w-[150px]">
            <Segmented
              label="Page size"
              value={a4 ? "a4" : "letter"}
              choices={[
                { value: "a4", label: "A4" },
                { value: "letter", label: "Letter" },
              ]}
              onChange={(v) => setA4(v === "a4")}
            />
          </div>
          {quickSheets && (
            <>
              <div className="w-[190px]">
                <Segmented
                  label="Which sheets"
                  value={firstSheetOnly ? "first" : "all"}
                  choices={[
                    { value: "all", label: "All sheets" },
                    { value: "first", label: "First sheet" },
                  ]}
                  onChange={(v) => setFirstSheetOnly(v === "first")}
                />
              </div>
              <div className="w-[180px]">
                <Segmented
                  label="Orientation"
                  value={landscape ? "landscape" : "portrait"}
                  choices={[
                    { value: "portrait", label: "Portrait" },
                    { value: "landscape", label: "Landscape" },
                  ]}
                  onChange={(v) => setLandscape(v === "landscape")}
                />
              </div>
            </>
          )}
        </div>
      )}

      <div className="flex gap-2">
        <Button onClick={onBack}>Back</Button>
        <Button type="submit" variant="primary" disabled={!infos || n === 0} className="flex-1" autoFocus>
          {label}
          {blocked.length > 0 && n > 0 && (
            <span className="opacity-70">
              (skips {blocked.map((g) => TYPE_LABEL[g.type]).join(", ")})
            </span>
          )}
        </Button>
      </div>
    </form>
  );
}

interface CardProps {
  engine: Engine;
  type: DocType;
  info?: EngineInfo;
  checking: boolean;
  selected: boolean;
  focused: boolean;
  onPick: () => void;
  onInfo: () => void;
  onStore: () => void;
  storeOpened: boolean;
  pkg: LibreOfficePackage | null;
  install: InstallProgress | null;
  confirming: boolean;
  onAskInstall: () => void;
  onConfirm: () => void;
  onNotNow: () => void;
  onCancelInstall: () => void;
  installing: boolean;
}

function EngineCard(props: CardProps) {
  const { engine, type, info, checking, selected, focused, onPick, onInfo, onStore, storeOpened, pkg, install, confirming } = props;
  const quick = engine === "quick";
  const isReady = quick || info?.installed === true;
  const name = ENGINE_NAME[engine];
  const title = quick ? "Quick (built in)" : `Exact copy with ${name}`;
  const status = quick
    ? "Ready now · close to the original, not 100% the same"
    : checking
      ? "Checking…"
      : isReady
        ? `${name}${info?.version ? ` ${info.version}` : ""} installed · exact copy`
        : `Not installed · install it once and QuickConvert uses it automatically`;
  const busy = install !== null && ["downloading", "checking", "installing"].includes(install.phase);

  return (
    <div
      className={`flex flex-col gap-1.5 rounded-control px-2.5 py-2 ${
        selected ? "bg-surface-selected inset-ring inset-ring-accent" : focused ? "bg-surface inset-ring inset-ring-separator" : "bg-surface"
      }`}
    >
      <div className="flex items-start gap-2">
        <button
          type="button"
          role="radio"
          aria-checked={selected}
          aria-disabled={!isReady}
          onClick={() => (isReady ? onPick() : onInfo())}
          className="flex min-w-0 flex-1 items-start gap-2 text-left"
        >
          <span
            aria-hidden
            className={`mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-pill ${
              selected ? "bg-accent text-on-accent" : "inset-ring inset-ring-separator"
            } ${isReady ? "" : "opacity-40"}`}
          >
            {selected && <Check size={11} strokeWidth={3} />}
          </span>
          <span className="flex min-w-0 flex-col">
            <span className={`type-body-strong ${selected ? "text-link" : isReady ? "text-primary" : "text-secondary"}`}>{title}</span>
            <span className="type-caption text-secondary">{status}</span>
          </span>
        </button>
        <button
          type="button"
          onClick={onInfo}
          aria-label={`About ${name}`}
          title={`About ${name}`}
          className="flex size-6 shrink-0 items-center justify-center rounded-pill text-secondary hover:bg-surface-hover"
        >
          <Info size={15} strokeWidth={1.75} />
        </button>
      </div>

      {!quick && !checking && !isReady && engine !== "libreoffice" && (
        <div className="flex items-center gap-2 pl-6">
          <Button onClick={onStore}>Install {name}</Button>
          <span className="type-caption text-tertiary">
            {storeOpened ? "In the App Store, click Get. QuickConvert notices when it's installed." : "Free · opens the App Store"}
          </span>
        </div>
      )}

      {engine === "libreoffice" && !checking && !isReady && (
        <div className="flex flex-col gap-1.5 pl-6" aria-live="polite">
          {busy && install ? (
            <InstallProgressRow install={install} onCancel={props.onCancelInstall} />
          ) : confirming ? (
            <>
              <span className="type-caption text-primary">
                Download LibreOffice {pkg?.version ?? ""} ({pkg ? megabytes(pkg.bytes) : "about 300"} MB) from libreoffice.org and install it in
                Applications?
              </span>
              <div className="flex gap-2">
                <Button variant="primary" onClick={props.onConfirm}>
                  Download and install
                </Button>
                <Button onClick={props.onNotNow}>Not now</Button>
              </div>
            </>
          ) : (
            <div className="flex items-center gap-2">
              <Button onClick={props.onAskInstall} disabled={props.installing}>
                Install LibreOffice
              </Button>
              <span className="type-caption text-tertiary">
                {install?.phase === "failed"
                  ? (install.message ?? "Couldn't install it.")
                  : install?.phase === "cancelled"
                    ? "Cancelled. Nothing was installed."
                    : `Free · ${pkg ? megabytes(pkg.bytes) : "about 300"} MB download`}
              </span>
            </div>
          )}
        </div>
      )}

      {engine === "libreoffice" && isReady && install?.phase === "done" && (
        <span className="type-caption pl-6 text-secondary">Installed. It's used for {TYPE_LABEL[type]} files from now on unless you pick another way.</span>
      )}
    </div>
  );
}

function InstallProgressRow({ install, onCancel }: { install: InstallProgress; onCancel: () => void }) {
  const percent = install.total > 0 ? Math.min(100, (install.received / install.total) * 100) : 0;
  const text =
    install.phase === "downloading"
      ? `Downloading… ${megabytes(install.received)} of ${megabytes(install.total)} MB`
      : install.phase === "checking"
        ? "Checking the download is genuine…"
        : "Installing into Applications…";
  return (
    <div className="flex flex-col gap-1">
      <div className="h-1 overflow-hidden rounded-pill bg-separator" role="progressbar" aria-valuenow={Math.round(percent)} aria-valuemin={0} aria-valuemax={100}>
        <div className="h-full rounded-pill bg-accent transition-[width]" style={{ width: `${install.phase === "downloading" ? percent : 100}%` }} />
      </div>
      <div className="flex items-center gap-2">
        <LoaderCircle size={12} strokeWidth={2} className="animate-spin text-tertiary motion-reduce:animate-none" />
        <span className="type-caption flex-1 text-secondary tabular-nums">{text}</span>
        {install.phase === "downloading" && (
          <button type="button" onClick={onCancel} className="type-caption text-link hover:underline">
            Cancel
          </button>
        )}
      </div>
    </div>
  );
}

/** The ⓘ panel: what it is, why, which version, how big, where from, and how to remove it. */
function DetailsPanel({ engine, type, info, pkg }: { engine: Engine; type: DocType; info?: EngineInfo; pkg: LibreOfficePackage | null }) {
  const d = DETAILS[engine];
  const quick = engine === "quick";
  const installed = quick || info?.installed === true;
  const version = quick
    ? "Part of QuickConvert"
    : installed
      ? `${ENGINE_NAME[engine]} ${info?.version ?? ""} is installed${info?.path ? ` (${info.path.replace(/\/[^/]+$/, "")})` : ""}`
      : engine === "libreoffice"
        ? `Will install LibreOffice ${pkg?.version ?? ""} for this Mac`
        : "The latest version from the App Store";
  const size =
    engine === "libreoffice" ? `${pkg ? megabytes(pkg.bytes) : "About 300"} MB download · about 900 MB once installed` : d.size;
  const why = quick ? (d.why[type] ?? "") : (d.why.all ?? "");

  const rows: [string, string][] = [
    ["What it is", d.what],
    [quick ? "What changes" : "Why it's better", why],
    ["Fonts", fontsNote(engine, type)],
    ["Version", version],
    ["Size", size],
    ["Comes from", d.from],
    ["Privacy", "Everything runs on your Mac. Your files are never uploaded."],
  ];
  if (!quick) {
    rows.push(["After installing", "QuickConvert uses it automatically. No setup needed."]);
    rows.push(["To remove it", "Drag it from Applications to the Bin."]);
  }
  return (
    <div className="flex flex-col gap-2.5" aria-live="polite">
      <div className="flex items-center gap-1.5">
        <h3 className="type-body-strong text-primary">{d.title}</h3>
        {installed && <Check size={13} strokeWidth={2.5} className="text-success" aria-label="Ready" />}
      </div>
      <dl className="flex flex-col gap-2">
        {rows.map(([label, text]) => (
          <div key={label} className="flex flex-col">
            <dt className="type-caption text-tertiary">{label}</dt>
            <dd className="type-caption text-primary">{text}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
