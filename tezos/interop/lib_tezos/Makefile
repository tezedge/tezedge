OBJDIR = artifacts
TEZOS_BASE_DIR?=src

all: package

package: build
		ar qs $(OBJDIR)/libtezos.a $(OBJDIR)/*.so

build: build-deps
		cd ${TEZOS_BASE_DIR} && opam config exec -- make
		mkdir -p $(OBJDIR)
		mv ${TEZOS_BASE_DIR}/libtezos-ffi.so $(OBJDIR)/libtezos.so

build-deps:
		cd ${TEZOS_BASE_DIR} && OPAMYES=1 make build-deps

clean:
		rm -rf artifacts
