// Switches between popup views and settings. M0: placeholder panel styled with design tokens.
function App() {
  return (
    <main className="flex h-screen items-center justify-center p-4">
      <div className="w-full rounded-popup bg-surface p-4 shadow-popup">
        <h1 className="type-title text-primary">QuickConvert</h1>
        <p className="type-caption mt-1 text-secondary">Scaffold ready. Popup arrives in M1.</p>
      </div>
    </main>
  );
}

export default App;
