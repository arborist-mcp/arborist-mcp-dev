typedef struct _object {
    long ob_refcnt;
    void *ob_type;
} PyObject;

void _Py_NewReference(PyObject *op);

int add(int left, int right) {
    return left + right;
}

