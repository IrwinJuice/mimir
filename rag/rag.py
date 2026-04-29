import os
import ollama
import chromadb

# ---- CONFIG ----
PROJECT_DIR = "/home/ij/workspace/mimir/"       # path to your cloned repo
EMBED_MODEL = "nomic-embed-text"
CHAT_MODEL  = "qwen3:14b"
EXTENSIONS  = {".rs", ".toml", ".sql", ".md"}
CHUNK_SIZE  = 60              # lines per chunk
CHUNK_OVERLAP = 10            # overlapping lines between chunks

# ---- 1. COLLECT FILES ----
def collect_files(root):
    files = []
    for dirpath, _, filenames in os.walk(root):
        # skip target/ build artifacts
        if "target" in dirpath.split(os.sep):
            continue
        for fname in filenames:
            if any(fname.endswith(ext) for ext in EXTENSIONS):
                files.append(os.path.join(dirpath, fname))
    return files

# ---- 2. CHUNK FILE INTO OVERLAPPING WINDOWS ----
def chunk_file(path, chunk_size=CHUNK_SIZE, overlap=CHUNK_OVERLAP):
    with open(path, "r", encoding="utf-8", errors="ignore") as f:
        lines = f.readlines()

    chunks = []
    step = chunk_size - overlap
    for i in range(0, max(1, len(lines)), step):
        chunk_lines = lines[i : i + chunk_size]
        text = "".join(chunk_lines).strip()
        if text:
            chunks.append({
                "text": text,
                "source": path,
                "start_line": i + 1,
            })
    return chunks

# ---- 3. INDEX INTO CHROMADB ----
def build_index(project_dir):
    client = chromadb.PersistentClient(path="./mimir_rag_db")

    # drop and recreate for a clean index
    try:
        client.delete_collection("mimir")
    except Exception:
        pass
    collection = client.create_collection("mimir")

    files = collect_files(project_dir)
    print(f"Found {len(files)} files")

    all_chunks = []
    for path in files:
        all_chunks.extend(chunk_file(path))

    print(f"Total chunks: {len(all_chunks)}")

    # embed in batches
    BATCH = 32
    for i in range(0, len(all_chunks), BATCH):
        batch = all_chunks[i : i + BATCH]
        texts = [c["text"] for c in batch]

        resp = ollama.embed(model=EMBED_MODEL, input=texts)
        embeddings = resp["embeddings"]

        collection.add(
            ids=[f"chunk_{i+j}" for j in range(len(batch))],
            embeddings=embeddings,
            documents=texts,
            metadatas=[{"source": c["source"], "line": c["start_line"]} for c in batch],
        )
        print(f"  Indexed {min(i+BATCH, len(all_chunks))}/{len(all_chunks)}")

    print("Index built!")
    return collection

# ---- 4. QUERY ----
def ask(collection, question):
    # embed question
    q_emb = ollama.embed(model=EMBED_MODEL, input=question)["embeddings"][0]

    # retrieve top 5 chunks
    results = collection.query(query_embeddings=[q_emb], n_results=5)
    docs = results["documents"][0]
    metas = results["metadatas"][0]

    context_parts = []
    for doc, meta in zip(docs, metas):
        context_parts.append(f"# {meta['source']} (line {meta['line']})\n{doc}")
    context = "\n\n---\n\n".join(context_parts)

    system = (
        "You are an expert Rust developer assistant for the Mimir project — "
        "a self-hosted bank transaction analytics tool. "
        "Answer questions using only the provided source code context. "
        "Be concise and precise. If the answer is not in the context, say so.\n\n"
        f"CONTEXT:\n{context}"
    )

    response = ollama.chat(
        model=CHAT_MODEL,
        messages=[
            {"role": "system", "content": system},
            {"role": "user",   "content": question},
        ],
    )
    return response["message"]["content"]

# ---- MAIN ----
if __name__ == "__main__":
    client = chromadb.PersistentClient(path="./mimir_rag_db")

    # Check if index already exists
    try:
        collection = client.get_collection("mimir")
        print(f"Loaded existing index ({collection.count()} chunks)")
    except Exception:
        collection = build_index(PROJECT_DIR)

    print("\nMimir RAG ready. Type 'exit' to quit.\n")
    while True:
        q = input("You: ").strip()
        if q.lower() in ("exit", "quit"):
            break
        if not q:
            continue
        answer = ask(collection, q)
        print(f"\nQwen3: {answer}\n")
