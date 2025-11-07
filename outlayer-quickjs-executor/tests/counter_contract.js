// counter contract in demo mode
// Uses near.storageRead/Write shim provided by the loader.
globalThis.increment = function () {
  let c = near.storageRead("count") || 0;
  c = (c|0) + 1;
  near.storageWrite("count", c);
  return { count: c };
};

globalThis.add = function (a, b) {
  const x = Number(a) || 0;
  const y = Number(b) || 0;
  return { sum: x + y };
};
