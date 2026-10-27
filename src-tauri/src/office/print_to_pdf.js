// Run by `osascript -l JavaScript` with: input path, output PDF path, paper width and height (pt).
// Reads a Word / RTF / OpenDocument / HTML file with macOS's own text engine and prints it to a
// paginated PDF: text starts at the top of the page, and anything wider than the page is scaled
// to fit its width.
ObjC.import("AppKit");

const MARGIN = 54; // 0.75 inch

function run(argv) {
  const [input, output, w, h] = argv;
  const width = Number(w);
  const height = Number(h);
  const text = $.NSAttributedString.alloc.initWithURLOptionsDocumentAttributesError(
    $.NSURL.fileURLWithPath(input),
    $({}),
    null,
    null,
  );
  if (text.isNil()) throw new Error("unreadable");

  const info = $.NSPrintInfo.sharedPrintInfo.copy;
  info.paperSize = $.NSMakeSize(width, height);
  info.orientation = width > height ? 1 : 0; // landscape : portrait
  info.topMargin = MARGIN;
  info.bottomMargin = MARGIN;
  info.leftMargin = MARGIN;
  info.rightMargin = MARGIN;
  info.verticallyCentered = false;
  info.horizontallyCentered = false;
  info.horizontalPagination = $.NSPrintingPaginationModeFit;
  info.verticalPagination = $.NSPrintingPaginationModeAutomatic;
  info.jobDisposition = $.NSPrintSaveJob;
  info.dictionary.setObjectForKey($.NSURL.fileURLWithPath(output), $.NSPrintJobSavingURL);

  const view = $.NSTextView.alloc.initWithFrame($.NSMakeRect(0, 0, width - 2 * MARGIN, 100));
  view.verticallyResizable = true;
  view.textStorage.setAttributedString(text);
  view.layoutManager.ensureLayoutForTextContainer(view.textContainer);
  const used = view.layoutManager.usedRectForTextContainer(view.textContainer);
  view.setFrameSize($.NSMakeSize(width - 2 * MARGIN, Math.max(used.size.height, 1)));

  const op = $.NSPrintOperation.printOperationWithViewPrintInfo(view, info);
  op.showsPrintPanel = false;
  op.showsProgressPanel = false;
  if (!op.runOperation) throw new Error("print failed");
  return "ok";
}
