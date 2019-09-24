OBJDIR = artifacts
TEZOS_BASE_DIR?=src

all: package

package: build
		ar qs $(OBJDIR)/libtezos.a $(OBJDIR)/*.o

build: build-deps
		cd ${TEZOS_BASE_DIR} && opam config exec -- make
		mkdir -p $(OBJDIR)
		mv ${TEZOS_BASE_DIR}/libtezos-ffi.so $(OBJDIR)/libtezos.o

build-deps:
		cd ${TEZOS_BASE_DIR} && make build-deps

clean:
		rm -rf artifacts
