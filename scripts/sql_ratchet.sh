#!/usr/bin/env bash
# TEXT-TO-SQL USABILITY PROOF — does our stack take a below-floor local 8B to USABLE on a real, wanted
# task, locally, zero-salary? Self-contained SQLite benchmark; FREE verifier = execute candidate SQL and
# compare result-set to gold. Floor-crossing pattern: 8B fails one-shot (research: 7B SLMs 4-12% on BIRD),
# a SCAFFOLD (schema-in-context + execution-repair, error-feedback only, no gold leak) crosses it, the
# free verifier keeps correct traces, distill into ONE-SHOT competence. PHASE=probe (default) measures
# floor + scaffold-cross before any LoRA; PHASE=full runs the ratchet. One model at a time.
set -uo pipefail; cd "$(dirname "$0")/.." || exit 1
export HF_HUB_DISABLE_PROGRESS_BARS=1
PY=.venv-mlx/bin/python; STUDENT=mlx-community/Qwen3-8B-4bit
W=/tmp/yogi_sql; mkdir -p "$W/data"; PHASE="${PHASE:-probe}"
"$PY" - "$W" "$STUDENT" "$PHASE" <<'PY'
import sqlite3,sys,re,os,json,subprocess,random
W,STU,PHASE=sys.argv[1],sys.argv[2],sys.argv[3]
DB=f"{W}/shop.db"
if os.path.exists(DB): os.remove(DB)
con=sqlite3.connect(DB)
con.executescript('''
CREATE TABLE customers(id INTEGER PRIMARY KEY, name TEXT, city TEXT, signup TEXT);
CREATE TABLE products(id INTEGER PRIMARY KEY, name TEXT, category TEXT, price REAL);
CREATE TABLE orders(id INTEGER PRIMARY KEY, customer_id INTEGER, order_date TEXT);
CREATE TABLE order_items(order_id INTEGER, product_id INTEGER, qty INTEGER);
INSERT INTO customers VALUES (1,'Alice','Paris','2024-01-05'),(2,'Bob','London','2024-02-11'),
 (3,'Carol','Paris','2023-11-20'),(4,'Dan','Berlin','2024-03-02'),(5,'Eve','London','2024-05-15'),
 (6,'Frank','Paris','2023-08-09'),(7,'Grace','Berlin','2024-06-01'),(8,'Heidi','London','2024-04-22');
INSERT INTO products VALUES (1,'Laptop','Electronics',1200.0),(2,'Phone','Electronics',800.0),
 (3,'Desk','Furniture',300.0),(4,'Chair','Furniture',150.0),(5,'Monitor','Electronics',250.0),
 (6,'Notebook','Stationery',5.0),(7,'Pen','Stationery',2.0),(8,'Lamp','Furniture',45.0);
INSERT INTO orders VALUES (1,1,'2024-03-01'),(2,1,'2024-04-10'),(3,2,'2024-04-15'),(4,3,'2023-12-01'),
 (5,4,'2024-05-20'),(6,5,'2024-06-05'),(7,1,'2024-06-18'),(8,6,'2024-02-02'),(9,2,'2024-07-01'),(10,7,'2024-07-09');
INSERT INTO order_items VALUES (1,1,1),(1,6,3),(2,2,1),(3,3,2),(3,4,4),(4,5,1),(5,1,1),(5,2,1),
 (6,7,10),(6,6,5),(7,8,2),(8,3,1),(9,2,2),(9,5,1),(10,4,1),(10,8,1);
'''); con.commit(); con.close()
SCHEMA=("customers(id,name,city,signup), products(id,name,category,price), "
        "orders(id,customer_id,order_date), order_items(order_id,product_id,qty)")
# (question, gold_sql) — hand-authored, varied difficulty. Verifier compares RESULT SETS.
QA=[
 ("How many customers are there?","SELECT COUNT(*) FROM customers"),
 ("List the names of products in the Electronics category.","SELECT name FROM products WHERE category='Electronics'"),
 ("What is the price of the Laptop?","SELECT price FROM products WHERE name='Laptop'"),
 ("How many customers are from Paris?","SELECT COUNT(*) FROM customers WHERE city='Paris'"),
 ("List all product categories (distinct).","SELECT DISTINCT category FROM products"),
 ("What is the most expensive product's name?","SELECT name FROM products ORDER BY price DESC LIMIT 1"),
 ("How many orders did Alice place?","SELECT COUNT(*) FROM orders o JOIN customers c ON o.customer_id=c.id WHERE c.name='Alice'"),
 ("What is the total number of items ordered (sum of qty)?","SELECT SUM(qty) FROM order_items"),
 ("Which city has the most customers?","SELECT city FROM customers GROUP BY city ORDER BY COUNT(*) DESC LIMIT 1"),
 ("List the names of customers who placed at least one order.","SELECT DISTINCT c.name FROM customers c JOIN orders o ON o.customer_id=c.id"),
 ("How many orders were placed in 2024?","SELECT COUNT(*) FROM orders WHERE order_date>='2024-01-01' AND order_date<'2025-01-01'"),
 ("What is the average product price?","SELECT AVG(price) FROM products"),
 ("Which products have never been ordered? List their names.","SELECT name FROM products WHERE id NOT IN (SELECT product_id FROM order_items)"),
 ("What is the total revenue (sum of qty*price across all order items)?","SELECT SUM(oi.qty*p.price) FROM order_items oi JOIN products p ON oi.product_id=p.id"),
 ("How many distinct products did Bob order?","SELECT COUNT(DISTINCT oi.product_id) FROM order_items oi JOIN orders o ON oi.order_id=o.id JOIN customers c ON o.customer_id=c.id WHERE c.name='Bob'"),
 ("Which customer placed the most orders? Give their name.","SELECT c.name FROM customers c JOIN orders o ON o.customer_id=c.id GROUP BY c.id ORDER BY COUNT(*) DESC LIMIT 1"),
 ("List product names that cost more than 200.","SELECT name FROM products WHERE price>200"),
 ("How many orders does each city's customers have in total? Give city and count.","SELECT c.city, COUNT(*) FROM customers c JOIN orders o ON o.customer_id=c.id GROUP BY c.city"),
 ("What is the name of the product with the highest total quantity ordered?","SELECT p.name FROM products p JOIN order_items oi ON oi.product_id=p.id GROUP BY p.id ORDER BY SUM(oi.qty) DESC LIMIT 1"),
 ("Which customers signed up in 2024? List names.","SELECT name FROM customers WHERE signup>='2024-01-01' AND signup<'2025-01-01'"),
 ("How many products are in each category? Give category and count.","SELECT category, COUNT(*) FROM products GROUP BY category"),
 ("What is the total revenue from Electronics products?","SELECT SUM(oi.qty*p.price) FROM order_items oi JOIN products p ON oi.product_id=p.id WHERE p.category='Electronics'"),
 ("List the names of customers who spent more than the average customer spend.","SELECT c.name FROM customers c JOIN orders o ON o.customer_id=c.id JOIN order_items oi ON oi.order_id=o.id JOIN products p ON oi.product_id=p.id GROUP BY c.id HAVING SUM(oi.qty*p.price) > (SELECT AVG(t) FROM (SELECT SUM(oi2.qty*p2.price) t FROM customers c2 JOIN orders o2 ON o2.customer_id=c2.id JOIN order_items oi2 ON oi2.order_id=o2.id JOIN products p2 ON oi2.product_id=p2.id GROUP BY c2.id))"),
 ("How many orders contain more than one distinct product?","SELECT COUNT(*) FROM (SELECT order_id FROM order_items GROUP BY order_id HAVING COUNT(DISTINCT product_id)>1)"),
]
random.seed(0); idx=list(range(len(QA))); random.shuffle(idx)
train=[QA[i] for i in idx[:16]]; test=[QA[i] for i in idx[16:]]
def run_sql(sql):
    if not re.match(r'(?is)^\s*select\b', sql.strip()): return ("ERR","not a SELECT")
    if re.search(r'(?i)\b(drop|delete|update|insert|attach|pragma|create|alter)\b', sql): return ("ERR","forbidden")
    try:
        c=sqlite3.connect(DB); rows=c.execute(sql).fetchall(); c.close(); return ("OK",rows)
    except Exception as e: return ("ERR",str(e)[:120])
def norm(rows): return sorted([tuple(str(x) for x in r) for r in rows])
def correct(cand,gold):
    s,c=run_sql(cand); s2,g=run_sql(gold)
    return s=="OK" and s2=="OK" and norm(c)==norm(g)
def extract(out):
    t=out.split('</think>')[-1]; m=re.search(r'```(?:sql)?\s*(.*?)```',t,re.S)
    sql=(m.group(1) if m else t).strip()
    m2=re.search(r'(?is)(select\b.*?)(;|$)',sql); return (m2.group(1).strip() if m2 else sql)
from mlx_lm import load,generate
def gen(model,tok,p,mx=220):
    return generate(model,tok,prompt=tok.apply_chat_template([{"role":"user","content":p}],add_generation_prompt=True,tokenize=False),max_tokens=mx,verbose=False)
oneshot=lambda q:f"SQLite schema: {SCHEMA}. Question: {q} Write ONE SQLite SELECT query that answers it. Output only the SQL. /no_think"
def scaffold(model,tok,q,gold):  # error-feedback repair (no gold leak), <=3 rounds
    sql=extract(gen(model,tok,oneshot(q)))
    for _ in range(2):
        if correct(sql,gold): return sql,True
        st,info=run_sql(sql)
        fb=(f"The query errored: {info}." if st=="ERR" else f"The query ran but returned {len(info)} rows and is not correct.")
        sql=extract(gen(model,tok,f"SQLite schema: {SCHEMA}. Question: {q} Your query: {sql} . {fb} Write a corrected single SELECT query. Output only SQL. /no_think"))
    return sql,correct(sql,gold)
adapter=f"{W}/adapter"
ad = adapter if (PHASE=="full" and os.path.exists(f"{adapter}/adapters.safetensors")) else None
model,tok=load(STU,adapter_path=None)
# PROBE: one-shot floor + scaffold-cross on the TRAIN set (also generates traces for full)
print(f"=== PROBE (8B, train n={len(train)}): one-shot floor vs scaffold-cross ===",flush=True)
oneshot_ok=0; scaf_ok=0; rows=[]
for q,gold in train:
    s1=extract(gen(model,tok,oneshot(q))); o1=correct(s1,gold); oneshot_ok+=o1
    sql,ok=scaffold(model,tok,q,gold); scaf_ok+=ok
    if ok: rows.append({"prompt":oneshot(q),"completion":" "+sql})
print(f"  one-shot (FLOOR): {oneshot_ok}/{len(train)}   scaffold-cross: {scaf_ok}/{len(train)}",flush=True)
print(f"  -> scaffold yields {len(rows)} verified one-shot traces to distill",flush=True)
# held-out one-shot floor (before)
to=sum(correct(extract(gen(model,tok,oneshot(q))),gold) for q,gold in test)
print(f"  HELD-OUT one-shot before: {to}/{len(test)}",flush=True)
if PHASE!="full":
    print("PROBE done. If floor low (one-shot << scaffold) -> run PHASE=full for the ratchet.",flush=True); sys.exit(0)
# FULL: distill scaffold-successes into one-shot, eval held-out after
d=f"{W}/data"; open(f"{d}/train.jsonl","w").write("\n".join(json.dumps(r) for r in rows)+"\n")
open(f"{d}/valid.jsonl","w").write("\n".join(json.dumps(r) for r in rows[:max(2,len(rows)//5)])+"\n")
del model,tok
os.makedirs(adapter,exist_ok=True)
subprocess.run([sys.executable,"-m","mlx_lm.lora","--model",STU,"--train","--data",d,"--batch-size","2",
  "--num-layers","16","--iters","200","--learning-rate","1e-4","--adapter-path",adapter],capture_output=True,text=True)
m1,t1=load(STU,adapter_path=adapter)
ta=sum(correct(extract(gen(m1,t1,oneshot(q))),gold) for q,gold in test)
print(f"\n=== SQL USABILITY PROOF RESULT (held-out one-shot execution accuracy) ===",flush=True)
print(f"  one-shot before {to}/{len(test)} -> after {ta}/{len(test)}  (scaffold internalized into one-shot competence?)",flush=True)
print(f"  USABLE ✓ iff one-shot after >> before (weak local 8B became usable at real SQL, locally, zero salary).",flush=True)
PY
