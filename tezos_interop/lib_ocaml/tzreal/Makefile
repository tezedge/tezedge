OBJDIR = artifacts
TEZOS_BASE_DIR?=tezos

all: package

package: build
		ar qs $(OBJDIR)/libtzreal.a $(OBJDIR)/*.o

build: build-deps
		cd ${TEZOS_BASE_DIR} && opam config exec -- make
		mkdir -p $(OBJDIR)
		mv ${TEZOS_BASE_DIR}/libtezos-ffi.so $(OBJDIR)/libtzreal.o

build-deps:
		cd ${TEZOS_BASE_DIR} && make build-deps

clean:
		cd ${TEZOS_BASE_DIR} && opam config exec -- make clean
		rm -rf artifacts
