OBJDIR = artifacts

all: package

package: build
		ar qs $(OBJDIR)/libtzmock.a $(OBJDIR)/*.o

build:
		opam config exec -- dune build tzmock.so
		mkdir -p $(OBJDIR)
		mv _build/default/tzmock.so $(OBJDIR)/libtzmock.o

clean:
		opam config exec -- dune clean
		rm -rf $(OBJDIR)
