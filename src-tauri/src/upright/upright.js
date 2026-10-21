// Run by `osascript -l JavaScript` with one image path (upright pixels, no orientation tag).
// Prints {"text":[…],"faces":[…]}: how convincing the image looks when read as is, then as if it
// needs a quarter turn right, a half turn, a quarter turn left. Uses Apple's Vision framework;
// nothing leaves the Mac.
ObjC.import("Foundation");
ObjC.import("AppKit");
ObjC.import("Vision");

// CGImagePropertyOrientation for each reading: up, right, down, left.
const ORIENTATIONS = [1, 6, 3, 8];
const speller = $.NSSpellChecker.sharedSpellChecker;

/** Letters in words the spell checker knows. Upside-down text reads as gibberish and scores ~0. */
function realLetters(text) {
  let total = 0;
  for (const word of text.split(/[^A-Za-z]+/)) {
    if (word.length < 3) continue;
    const miss = speller.checkSpellingOfStringStartingAt($(word), 0);
    if (Number(miss.length) === 0) total += word.length;
  }
  return total;
}

function perform(url, orientation, request) {
  const handler = $.VNImageRequestHandler.alloc.initWithURLOrientationOptions(url, orientation, $({}));
  return handler.performRequestsError($([request]), null) ? request.results : null;
}

function textScore(url, orientation) {
  const request = $.VNRecognizeTextRequest.alloc.init;
  request.recognitionLevel = 1; // fast: reads only text that is the right way up
  request.usesLanguageCorrection = false;
  const results = perform(url, orientation, request);
  let score = 0;
  for (let i = 0; results && i < results.count; i++) {
    const best = results.objectAtIndex(i).topCandidates(1).firstObject;
    if (!best.isNil()) score += realLetters(best.string.js);
  }
  return score;
}

/** Face confidence, weighted by how level the faces are in this reading. */
function faceScore(url, orientation) {
  const results = perform(url, orientation, $.VNDetectFaceRectanglesRequest.alloc.init);
  let score = 0;
  for (let i = 0; results && i < results.count; i++) {
    const face = results.objectAtIndex(i);
    const level = face.roll.isNil() ? 1 : Math.max(0, Math.cos(face.roll.doubleValue));
    score += face.confidence * level;
  }
  return score;
}

function run(argv) {
  const url = $.NSURL.fileURLWithPath(argv[0]);
  return JSON.stringify({
    text: ORIENTATIONS.map((o) => textScore(url, o)),
    faces: ORIENTATIONS.map((o) => faceScore(url, o)),
  });
}
