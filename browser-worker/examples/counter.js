// Minimal counter contract example
globalThis.increment = function () {
  let c = near.storageRead("count") || 0;
  c = (c|0) + 1;
  near.storageWrite("count", c);
  near.log("count ->", c);
  return { count: c };
};

globalThis.getValue = function () {
  let c = near.storageRead("count") || 0;
  return { count: c };
};

globalThis.reset = function () {
  near.storageWrite("count", 0);
  near.log("count reset to 0");
  return { count: 0 };
};
