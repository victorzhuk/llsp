#![allow(dead_code)]

pub const SAMPLES: &[(&str, &str)] = &[
    (
        "common-lisp",
        "(defpackage :bench (:use :cl))\n(in-package :bench)\n\n(defun fib-{i} (n &optional (acc 0))\n  \"Compute something.\"\n  (declare (type fixnum n))\n  (if (< n 2)\n      (+ n acc)\n      (let ((a (fib-{i} (- n 1)))\n            (b #'car))\n        (loop for x in '(1 2 3) collect (* x a) #| block |#)))) ; tail\n\n",
    ),
    (
        "clojure",
        "(ns bench.core-{i}\n  (:require [clojure.string :as str]))\n\n(defn handler-{i}\n  \"Handles a request.\"\n  [{:keys [params] :as req}]\n  (let [id (get params :id)\n        m {:a 1, :b [1 2 3] :c #{:x :y}}]\n    #_(println req)\n    (-> m (assoc :id id) (update :a inc) (str/join))))\n\n",
    ),
    (
        "scheme",
        "(define (fold-{i} f acc lst)\n  (if (null? lst)\n      acc\n      (fold-{i} f (f acc (car lst)) (cdr lst))))\n\n(define-record-type point (make-point x y) point? (x point-x) (y point-y))\n(let loop ((i 0)) (when (< i 10) (display #\\a) (loop (+ i 1)))) #;(ignored)\n\n",
    ),
    (
        "racket",
        "#lang racket\n(define (sum-{i} xs)\n  (for/fold ([acc 0]) ([x (in-list xs)])\n    (+ acc x)))\n(struct posn (x y) #:transparent)\n(define h #hash((a . 1) (b . 2)))\n\n",
    ),
    (
        "emacs-lisp",
        "(defun my-cmd-{i} (arg)\n  \"Docstring.\"\n  (interactive \"p\")\n  (let ((c ?a) (v [1 2 3]))\n    (when (> arg 0)\n      (message \"%s %c\" v c)\n      (mapcar #'1+ '(1 2 3)))))\n\n",
    ),
    (
        "fennel",
        "(fn step-{i} [state dt]\n  (let [{: x : y} state]\n    (each [k v (pairs state)]\n      (print k v))\n    #(+ $1 dt)))\n\n",
    ),
    (
        "janet",
        "(defn step-{i} [state dt]\n  # comment\n  (def xs @[1 2 3])\n  (each x xs (print x))\n  (let [s ``long string``] (string s dt)))\n\n",
    ),
];

pub fn generate(template: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + template.len());
    let mut i = 0;
    while out.len() < bytes {
        out.push_str(&template.replace("{i}", &i.to_string()));
        i += 1;
    }
    out
}
