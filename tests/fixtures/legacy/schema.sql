CREATE TABLE Main(
  lID INTEGER PRIMARY KEY AUTOINCREMENT,
  lDate INTEGER,
  mText TEXT,
  lShortCut INTEGER,
  lDontAutoDelete INTEGER,
  CRC INTEGER,
  bIsGroup INTEGER,
  lParentID INTEGER,
  QuickPasteText TEXT,
  clipOrder REAL,
  clipGroupOrder REAL,
  globalShortCut INTEGER,
  lastPasteDate INTEGER,
  stickyClipOrder REAL,
  stickyClipGroupOrder REAL,
  MoveToGroupShortCut INTEGER,
  GlobalMoveToGroupShortCut INTEGER
);

CREATE TABLE Data(
  lID INTEGER PRIMARY KEY AUTOINCREMENT,
  lParentID INTEGER,
  strClipBoardFormat TEXT,
  ooData BLOB
);

CREATE TABLE Types(
  lID INTEGER PRIMARY KEY AUTOINCREMENT,
  TypeText TEXT
);

CREATE TABLE CopyBuffers(
  lID INTEGER PRIMARY KEY AUTOINCREMENT,
  lClipID INTEGER,
  lCopyBuffer INTEGER
);

CREATE TABLE MainDeletes(
  clipID INTEGER,
  modifiedDate TEXT
);

CREATE UNIQUE INDEX Main_ID ON Main(lID);
CREATE UNIQUE INDEX Data_ID ON Data(lID);
CREATE INDEX Main_ParentId ON Main(lParentID);
CREATE INDEX Data_ParentId_Format ON Data(lParentID, strClipBoardFormat);
