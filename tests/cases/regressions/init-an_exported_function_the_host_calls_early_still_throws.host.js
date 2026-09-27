globalThis.hostCall = (f) => {
  try {
    console.log(String(f()));
  } catch (error) {
    console.log(error instanceof ReferenceError ? "dead zone" : "other");
  }
};
